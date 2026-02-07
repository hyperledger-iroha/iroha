---
id: registrar-api
lang: kk
direction: ltr
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Name Service Registrar API & Governance Hooks
sidebar_label: Registrar API
description: Torii REST/gRPC surfaces, Norito DTOs, and governance artifacts for SNS registrations (SN-2b).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ескерту Канондық дереккөз
Бұл бет `docs/source/sns/registrar_api.md` бейнесін көрсетеді және қазір ол ретінде қызмет етеді
канондық портал көшірмесі. Бастапқы файл аударма жұмыс үрдістері үшін қалады.
:::

# SNS Registrar API & Governance Hooks (SN-2b)

**Күйі:** 24.03.2026 ж. әзірленген -- Nexus Негізгі шолу бойынша  
**Жол картасы сілтемесі:** SN-2b «Тіркеуші API және басқару ілмектері»  
**Алғышарттар:** [`registry-schema.md`](./registry-schema.md) ішіндегі схема анықтамалары

Бұл ескертпе Torii соңғы нүктелерін, gRPC қызметтерін, сұрау/жауап DTOларын және Sora Name Service (SNS) тіркеушісін пайдалану үшін қажетті басқару артефактілерін көрсетеді. Бұл SNS атауларын тіркеуді, жаңартуды немесе басқаруды қажет ететін SDK, әмияндар және автоматтандыруға арналған беделді келісімшарт.

## 1. Тасымалдау және аутентификация

| Талап | Толығырақ |
|-------------|--------|
| Протоколдар | `/v1/sns/*` және gRPC қызметі `sns.v1.Registrar` астында REST. Екеуі де Norito-JSON (`application/json`) және Norito-RPC екілік (`application/x-norito`) қабылдайды. |
| Auth | `Authorization: Bearer` таңбалауыштары немесе mTLS сертификаттары әр жұрнақ басқарушыға берілген. Басқаруға сезімтал соңғы нүктелер (қаттыру/қаттыру, резервтелген тағайындаулар) `scope=sns.admin` талап етеді. |
| Тарифтік шектеулер | Тіркеушілер `torii.preauth_scheme_limits` шелектерін JSON қоңырау шалушылармен және әр жұрнақ үшін басылған шектеулермен бөліседі: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Телеметрия | Torii тіркеуші өңдеушілері үшін `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` көрсетеді (`scheme="norito_rpc"` бойынша сүзгі); API сонымен қатар `sns_registrar_status_total{result, suffix_id}` көбейтеді. |

## 2. DTO шолуы

Өрістер [`registry-schema.md`](./registry-schema.md) ішінде анықталған канондық құрылымдарға сілтеме жасайды. Барлық пайдалы жүктемелер анық емес бағыттауды болдырмау үшін `NameSelectorV1` + `SuffixId` кірістіреді.

```text
Struct RegisterNameRequestV1 {
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,                     // 1..=max_term_years
    pricing_class_hint: Option<u8>,     // steward-advertised tier id
    payment: PaymentProofV1,
    governance: GovernanceHookV1,
    metadata: Metadata,
}

Struct RegisterNameResponseV1 {
    name_record: NameRecordV1,
    registry_event: RegistryEventV1,
    revenue_accrual: RevenueAccrualEventV1,
}

Struct PaymentProofV1 {
    asset_id: AssetId,
    gross_amount: TokenValue,
    net_amount: TokenValue,
    settlement_tx: Hash,
    payer: AccountId,
    signature: Signature,               // steward/treasury cosign
}

Struct GovernanceHookV1 {
    proposal_id: String,
    council_vote_hash: Hash,
    dao_vote_hash: Hash,
    steward_ack: Signature,
    guardian_clearance: Option<Signature>,
}

Struct RenewNameRequestV1 {
    selector: NameSelectorV1,
    term_years: u8,
    payment: PaymentProofV1,
}

Struct TransferNameRequestV1 {
    selector: NameSelectorV1,
    new_owner: AccountId,
    governance: GovernanceHookV1,
}

Struct UpdateControllersRequestV1 {
    selector: NameSelectorV1,
    controllers: Vec<NameControllerV1>,
}

Struct FreezeNameRequestV1 {
    selector: NameSelectorV1,
    reason: String,
    until: Timestamp,
    guardian_ticket: Signature,
}

Struct ReservedAssignmentRequestV1 {
    selector: NameSelectorV1,
    reserved_label: ReservedNameV1,
    governance: GovernanceHookV1,
}
```

## 3. REST соңғы нүктелері

| Соңғы нүкте | Әдіс | Жүктеме | Сипаттама |
|----------|--------|---------|-------------|
| `/v1/sns/registrations` | POST | `RegisterNameRequestV1` | Тіркеу немесе атауды қайта ашыңыз. Баға деңгейін шешеді, төлем/басқару дәлелдерін тексереді, тізілім оқиғаларын шығарады. |
| `/v1/sns/registrations/{selector}/renew` | POST | `RenewNameRequestV1` | Мерзімді ұзарту. Саясаттан жеңілдіктер/өткізу терезелерін қолданады. |
| `/v1/sns/registrations/{selector}/transfer` | POST | `TransferNameRequestV1` | Басқару мақұлдаулары тіркелгеннен кейін меншік құқығын беріңіз. |
| `/v1/sns/registrations/{selector}/controllers` | PUT | `UpdateControllersRequestV1` | Контроллер жинағын ауыстырыңыз; қол қойылған тіркелгі мекенжайларын растайды. |
| `/v1/sns/registrations/{selector}/freeze` | POST | `FreezeNameRequestV1` | Қамқоршы/кеңес қатып қалады. Қамқоршы билеті мен басқару құжатына сілтеме қажет. |
| `/v1/sns/registrations/{selector}/freeze` | ЖОЮ | `GovernanceHookV1` | Қалпына келтіруден кейін мұздатыңыз; кеңестің алдын ала жазылуын қамтамасыз етеді. |
| `/v1/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | Сақталған атауларды басқарушы/кеңес тағайындау. |
| `/v1/sns/policies/{suffix_id}` | АЛУ | — | `SuffixPolicyV1` токты алу (кэштеуге болады). |
| `/v1/sns/registrations/{selector}` | АЛУ | — | Ағымдағы `NameRecordV1` + тиімді күйді қайтарады (Белсенді, Grace, т.б.). |

**Таңдаушы кодтауы:** `{selector}` жол сегменті ADDR-5 үшін IH58 (қалаулы), қысылған (`sora`, екінші ең жақсы) немесе канондық он алтылықты қабылдайды; Torii оны `NameSelectorV1` арқылы қалыпқа келтіреді.

**Қате үлгісі:** барлық соңғы нүктелер `code`, `message`, `details` бар Norito JSON қайтарады. Кодтарға `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing` кіреді.

### 3.1 CLI көмекшілері (N0 қолмен тіркеуші талабы)

Жабық бета стюардтары енді тіркеушіні CLI арқылы қолмен JSON жасамай-ақ пайдалана алады:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id xor#sora \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` әдепкі бойынша CLI конфигурация тіркелгісі; Қосымша контроллер тіркелгілерін тіркеу үшін `--controller` қайталаңыз (әдепкі `[owner]`).
- Inline төлем жалаулары тікелей `PaymentProofV1` картасына; құрылымдық түбіртек болған кезде `--payment-json PATH` өтіңіз. Метадеректер (`--metadata-json`) және басқару ілгектері (`--governance-json`) бірдей үлгі бойынша жүреді.

Тек оқуға арналған көмекшілер жаттығуларды аяқтайды:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Іске асыру үшін `crates/iroha_cli/src/commands/sns.rs` қараңыз; пәрмендер осы құжатта сипатталған Norito DTO файлдарын қайта пайдаланады, осылайша CLI шығысы Torii жауаптарына байт байт сәйкес келеді.

Қосымша көмекшілер жаңартуларды, аударымдарды және қамқоршылық әрекеттерді қамтиды:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id xor#sora \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner ih58... \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows
iroha sns freeze \
  --selector makoto.sora \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.sora \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` жарамды `GovernanceHookV1` жазбасын қамтуы керек (ұсыныс идентификаторы, дауыс хэштері, басқарушы/қамқоршы қолдары). Әрбір пәрмен жай ғана сәйкес `/v1/sns/registrations/{selector}/…` соңғы нүктесін көрсетеді, осылайша бета операторлары SDK шақыратын нақты Torii беттерін қайталай алады.

## 4. gRPC қызметі

```text
service Registrar {
    rpc Register(RegisterNameRequestV1) returns (RegisterNameResponseV1);
    rpc Renew(RenewNameRequestV1) returns (NameRecordV1);
    rpc Transfer(TransferNameRequestV1) returns (NameRecordV1);
    rpc UpdateControllers(UpdateControllersRequestV1) returns (NameRecordV1);
    rpc Freeze(FreezeNameRequestV1) returns (NameRecordV1);
    rpc Unfreeze(GovernanceHookV1) returns (NameRecordV1);
    rpc AssignReserved(ReservedAssignmentRequestV1) returns (NameRecordV1);
    rpc GetRegistration(NameSelectorV1) returns (NameRecordV1);
    rpc GetPolicy(SuffixId) returns (SuffixPolicyV1);
}
```

Сым пішімі: компиляция уақыты Norito схема хэші астында жазылған
`fixtures/norito_rpc/schema_hashes.json` (`RegisterNameRequestV1` жолдары,
`RegisterNameResponseV1`, `NameRecordV1`, т.б.).

## 5. Governance Hooks & Evidence

Әрбір мутацияға ұшыраған қоңырау қайталау үшін жарамды дәлелдерді қосуы керек:

| Әрекет | Қажетті басқару деректері |
|--------|-------------------------|
| Стандартты тіркеу/жаңарту | Есеп айырысу нұсқауына сілтеме жасайтын төлемді растайтын құжат; деңгей басқарушы мақұлдауын талап етпесе, кеңес дауысы қажет емес. |
| Премиум деңгей тізілімі / резервтелген тапсырма | `GovernanceHookV1` ұсыныс идентификаторына сілтеме + басқарушының растауы. |
| Тасымалдау | Кеңес дауыс хэш + DAO сигнал хэш; дауды шешуге байланысты ауыстыру кезінде қамқоршыны босату. |
| Мұздату/Мұздату | Қамқоршы билетінің қолы және кеңестің қайта анықтауы (мұздатуды босату). |

Torii мыналарды тексеру арқылы дәлелдемелерді тексереді:

1. Ұсыныс идентификаторы басқару кітапшасында (`/v1/governance/proposals/{id}`) бар және күйі `Approved`.
2. Хэштер жазылған дауыс артефактілеріне сәйкес келеді.
3. Басқарушы/қамқоршы қолдары `SuffixPolicyV1` күтілетін ашық кілттерге сілтеме жасайды.

Сәтсіз тексерулер `sns_err_governance_missing` қайтарады.

## 6. Жұмыс үрдісінің мысалдары

### 6.1 Стандартты тіркеу

1. Клиент бағаны, жеңілдікті және қолжетімді деңгейлерді алу үшін `/v1/sns/policies/{suffix_id}` сұрайды.
2. Клиент `RegisterNameRequestV1` құрастырады:
   - `selector` таңдаулы IH58 немесе екінші ең жақсы қысылған (`sora`) жапсырмасынан алынған.
   - `term_years` саясат шегінде.
   - `payment` қазынашылық/басқарушы сплиттерді аударуға сілтеме жасайды.
3. Torii растайды:
   - Белгіні қалыпқа келтіру + сақталған тізім.
   - `PriceTierV1` қарсы мерзімді/жалпы баға.
   - Төлемді растайтын сома >= есептелген баға + алымдар.
4. Сәттілік туралы Torii:
   - `NameRecordV1` сақталады.
   - `RegistryEventV1::NameRegistered` шығарады.
   - `RevenueAccrualEventV1` шығарады.
   - Жаңа жазба + оқиғаларды қайтарады.

### 6.2 Грант кезінде жаңарту

Жеңілдікті жаңарту стандартты сұрауды және айыппұлды анықтауды қамтиды:

- Torii `now` және `grace_expires_at` тексереді және `SuffixPolicyV1` қосымша ақы кестелерін қосады.
- Төлемді растау қосымша төлемді қамтуы керек. Сәтсіздік => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` жаңа `expires_at` жазады.

### 6.3 Guardian Freeze & Council override

1. Қамқоршы билет сілтемесі оқиға идентификаторымен `FreezeNameRequestV1` жібереді.
2. Torii жазбаны `NameStatus::Frozen` параметріне жылжытады, `NameFrozen` шығарады.
3. Түзеткеннен кейін кеңестің мәселелері шешіледі; оператор DELETE `/v1/sns/registrations/{selector}/freeze` жолын `GovernanceHookV1` арқылы жібереді.
4. Torii қайта анықтауды растайды, `NameUnfrozen` шығарады.

## 7. Тексеру және қате кодтары

| Код | Сипаттама | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Белгі сақталған немесе блокталған. | 409 |
| `sns_err_policy_violation` | Термин, деңгей немесе контроллер жинағы саясатты бұзады. | 422 |
| `sns_err_payment_mismatch` | Төлемді растайтын мән немесе актив сәйкессіздігі. | 402 |
| `sns_err_governance_missing` | Қажетті басқару артефактілері жоқ/жарамсыз. | 403 |
| `sns_err_state_conflict` | Ағымдағы өмірлік цикл күйінде операцияға рұқсат етілмейді. | 409 |

Барлық кодтар `X-Iroha-Error-Code` және құрылымдық Norito JSON/NRPC конверттері арқылы шығады.

## 8. Іске асыру туралы ескертпелер

- Torii `NameRecordV1.auction` бойынша күтудегі аукциондарды сақтайды және `PendingAuction` кезінде тікелей тіркеу әрекеттерін қабылдамайды.
- Төлем дәлелдері Norito бухгалтерлік түбіртектерді қайта пайдаланады; қазынашылық қызметтер көмекші API (`/v1/finance/sns/payments`) ұсынады.
- SDK бұл соңғы нүктелерді қатты терілген көмекшілермен орау керек, осылайша әмияндар қатенің анық себептерін көрсете алады (`ERR_SNS_RESERVED`, т.б.).

## 9. Келесі қадамдар

- SN-3 аукционына түскеннен кейін Torii өңдеушілерін нақты тізілім келісімшартына қосыңыз.
- Осы API-ге сілтеме жасайтын SDK-арнайы нұсқаулықтарды (Rust/JS/Swift) жариялаңыз.
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) басқару ілгегі дәлел өрістеріне көлденең сілтемелермен кеңейтіңіз.