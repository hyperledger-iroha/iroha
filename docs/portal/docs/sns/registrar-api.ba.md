---
lang: ba
direction: ltr
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 300ae819b5315b1ae4edab6caffc936e506c0d550a1ba75be1ace4d42f8e0b11
source_last_modified: "2026-01-28T17:11:30.701656+00:00"
translation_last_reviewed: 2026-02-07
id: registrar-api
title: Sora Name Service Registrar API & Governance Hooks
sidebar_label: Registrar API
description: Torii REST/gRPC surfaces, Norito DTOs, and governance artifacts for SNS registrations (SN-2b).
translator: machine-google-reviewed
---

:::иҫкәртергә канонлы сығанаҡ
Был бит көҙгө I18NI00000000300Х һәм хәҙер хеҙмәт итә
канон портал күсермәһе. Сығанаҡ файлы тәржемә эш ағымы өсөн ҡала.
::: 1990 й.

# SNS теркәүсе API & Идара итеү ҡармаҡтары (SN-2b)

**Статус:** 2026-03-24 - I18NT000000008X Core обзоры буйынса.  
**Род картаһы һылтанмаһы:** SN-2b “Яҙма API & идара итеү ҡармаҡтары”  
**Бөтәһе лә:** Схема билдәләмәләре [I18NI000000031X] (./registry-schema.md)

Был иҫкәрмә I18NT000000009X ос нөктәләре, gRPC хеҙмәттәре, запрос/яуап DTOs, һәм идара итеү артефакттарын күрһәтеү өсөн кәрәкле Сора исеме хеҙмәте (SNS) регистраторы. Ул SDKs, янсыҡтар һәм автоматлаштырыу өсөн авторитетлы килешәү, уларҙы теркәргә, яңыртырға йәки SNS исемдәре менән идара итергә тейеш.

## 1. Транспорт һәм аутентификация

| Талап | Ентекле |
|------------|---------|
| Протоколдар | REST буйынса `/v1/sns/*` һәм gRPC хеҙмәте `sns.v1.Registrar`. Икеһе лә ҡабул итә I18NT0000000000000X-JSON (I18NI000000034X) һәм I18NT0000000001X-RPC бинар (I18NI000000035X). |
| Аут | I18NI000000036X токендар йәки mTLS сертификаттары бирелгән ялғау стеуард. Идара итеүгә һиҙгер ос нөктәләре (туңдырыу/туңдырмау, запас заданиелар) `scope=sns.admin` талап итә. |
| Сиктәре сиктәре | Регистраторҙар I18NI000000038X биҙрәләре менән JSON шылтыратыусылар менән бүлешә плюс суффикс ярсыҡтары: I18NI0000000039X, `sns.renew`, I18NI0000000041X, I18NI0000000042X. |
| Телеметрия | Torii `torii_request_duration_seconds{scheme}` фашлай / I18NI000000044X регистратор менән эш итеүселәр өсөн (фильтр `scheme="norito_rpc"`); API шулай уҡ `sns_registrar_status_total{result, suffix_id}`X өҫтәүҙәр. |

## 2. ДТО Обзор

Баҫыуҙар [I18NI000000047X] (I18NU000000028X X) билдәләнгән канон структурҙарына һылтанма яһай). Бөтә файҙалы йөктәр `NameSelectorV1` + I18NI0000000049X X XIX маршрутлаштырыуҙан ҡотолоу өсөн индерелгән.

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

## 3. REST endpoints

| Аҙаҡҡы нөктә | Ысул | Түләү | Тасуирлама |
|---------|---------|----------|------------- |
| `/v1/sns/names` | POST | `RegisterNameRequestV1` | Исем йәки яңынан асыу исем. Хаҡтар ярусын хәл итә, түләү/идара итеү дәлилдәрен раҫлай, реестр ваҡиғаларын сығара. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POST | `RenewNameRequestV1` | Оҙайтыу срогы. Сәйәсәттән рәхмәт/ҡотҡарыу тәҙрәләрен үтәй. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POST | `TransferNameRequestV1` | Күсерергә милекселек бер тапҡыр идара итеү раҫлау беркетергә. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | ПУТ | `UpdateControllersRequestV1` | Контроллер комплектын алмаштырыу; раҫлау өсөн ҡул ҡуйылған иҫәп-хисап адрестары. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POST | `FreezeNameRequestV1` | Һаҡсы/кәңәш туңдырыу. Опекун билеты һәм идара итеү докетына һылтанма талап итә. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | ДЕЛЕТ | `GovernanceHookV1` | Ремедиациянан һуң туңдырмау; совет өҫтөнән теркәүен тәьмин итә. |
| `/v1/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | Стюард/кәңәшмә запас исемдәрҙе тәғәйенләү. |
| `/v1/sns/policies/{suffix_id}` | GET | — | Фетч ток I18NI000000065X (качина). |
| `/v1/sns/names/{namespace}/{literal}` | GET | — | Ҡайтарыу ағымдағы `NameRecordV1` + һөҙөмтәле дәүләт (Әүҙем, Грейс һ.б.). |

**Һайлаусы кодлау:** I18NI000000068X юл сегменты I105 (өҫтөнлөклө), ҡыҫылған (`sora`, икенсе иң яҡшы), йәки канонлы гекс бер ADDR-5; I18NT0000001X уны I18NI000000070X аша нормалаштыра.

**Хата моделе:** бөтә ос нөктәләре ҡайтарыу I18NT0000000002X JSON менән I18NI000000071X, `message`, `details`. Кодтар араһында `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI ярҙамсылары (N0 ҡул менән регистратор талап)

Ябыҡ бета стюардтар хәҙер CLI аша регистраторҙы ҡул эштәре JSON-ыһыҙ ҡуллана ала:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` CLI конфигурация иҫәбенә ғәҙәттәгесә; ҡабатлау I18NI000000079X өҫтәмә контроллер иҫәптәрен беркетергә (дефолт `[owner]`).
- Интернет түләү флагтары картаһы туранан-тура `PaymentProofV1`; үткәреү `--payment-json PATH`, ҡасан һеҙ инде структуралы квитанция. Метадата (I18NI0000083X) һәм идара итеү ҡармаҡтары (`--governance-json`) шул уҡ ҡалып буйынса.

Репетицияларҙы уҡыу өсөн генә уҡыу: репетициялар:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Ҡара: `crates/iroha_cli/src/commands/sns.rs` тормошҡа ашырыу өсөн; был документта һүрәтләнгән I18NT000000003X ДТО-ларҙы командалар ҡабаттан ҡуллана, шуға күрә CLI сығышы I18NT000000012X яуаптар байт-байтлы.

Өҫтәмә ярҙамсылар яңыртыу, күсермәләр, һәм опекун ғәмәлдәрен ҡаплай:

I18NF000000025X

I18NI000000086X ғәмәлдәге `GovernanceHookV1` рекорды (тәҡдим id, тауыш биреүҙең хештары, стюард/опекун ҡултамғалары) булырға тейеш. Һәр команда ябай ғына көҙгө I18NI000000088X ос нөктәһе шулай бета-операторҙар репетициялай ала теүәл I18NT000000013X өҫтө SDKs шылтыратасаҡ.

## 4. gRPC хеҙмәте

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

Сым-формат: компиляция-ваҡыт I18NT00000000004X схемаһы хеш теркәлгән.
I18NI000000089XX ( 2-се рәттә I18NI000000090X,
`RegisterNameResponseV1`, `NameRecordV1` һ.б.).

## 5. Идара итеү ҡармаҡтары & Дәлилдәр

Һәр мутация шылтыратыуы реплей өсөн яраҡлы дәлилдәрҙе беркетергә тейеш:

| Эш | Кәрәкле идара итеү мәғлүмәттәре |
|-------|-------------------------|
| Стандарт теркәү/яңыртыу | Түләү иҫбатлау һылтанмалар иҫәп-хисап инструкцияһы; бер ниндәй ҙә совет тауышы кәрәкмәй, әгәр ярус стеуард раҫлау талап итә. |
| Премиум ярус реестры / запас задание | I18NI000000093X һылтанмалар тәҡдиме id + стеуард таныу. |
| Трансфер | Совет тауыш биреү хеш + DAO сигнал хеш; опекун рөхсәт ҡасан күсерелгән бәхәстәрҙе хәл итеү арҡаһында башлана. |
| Туңдырыу/Атлама | Һаҡсы билет ҡултамғаһы плюс совет өҫтөнән идара итеү (незаятельный). |

I18NT000000014X тикшергәндән һуң дәлилдәрҙе раҫлай:

1. Тәҡдим id идара итеү китабында бар (`/v1/governance/proposals/{id}`) һәм статусы I18NI000000955X.
2. Хэшс теркәлгән тауыш артефакттарына тап килә.
3. Стюард/опекун ҡултамғалары I18NI0000000966-нан көтөлгән асыҡ асҡыстарға һылтанма яһай.

Уңышһыҙ чектар ҡайтарыу I18NI000000097X.

## 6. Эш ағымы миҫалдары

### 6.1 Стандарт теркәү

1. Клиент һорауҙары I18NI000000098X тиклем хаҡтарҙы алыу, рәхмәт, һәм мөмкин ярустары.
2. Клиент I18NI000000099Х төҙөй:
   - I105 йәки икенсе иң яҡшы ҡыҫылған (I18NI0000010101X) лейблынан алынған I18NI0000100X.
   - `term_years` сәйәсәт сиктәрендә.
   - `payment` ҡаҙна/стуард сплиттер тапшырыуына һылтанма яһап.
3. I18NT000000015X раҫлай:
   - Ярлыҡ нормалаштырыу + запас исемлеге.
   - Срок/киҫәк хаҡы vs `PriceTierV1`.
   - Түләү иҫбатлау суммаһы >= иҫәпләнгән хаҡ + түләүҙәр.
4. Уңыш тураһында I18NT000000016X:
   - Презист `NameRecordV1`.
   - Эмиттар `RegistryEventV1::NameRegistered`.
   - Эмиттар `RevenueAccrualEventV1`.
   - Яңы рекорд + ваҡиғаларҙы кире ҡайтара.

### 6.2 Грейс ваҡытында яңыртыу

Грейс яңыртыу стандарт запрос плюс штраф асыҡлау инә:

- Torii чектары I18NI000000108X vs I18NI000000109X X һәм I18NI000000110X өҫтәмә өҫтәлдәр өҫтәй.
- Түләү дәлилдәре өҫтәмә түләргә тейеш. Уңышһыҙлыҡ => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` яңы I18NI000000113X яңы теркәлә.

### 6.3 Һаҡсы туңдырыу & Совет өҫтөнлөк

1. Guardian тапшыра I18NI000000114X менән билет һылтанмалар инцидент id.
.
3. Ремедиациянан һуң совет мәсьәләләре өҫтөнлөк итә; операторы I18NI0000000118X менән ДЕЛЕТ `/v1/sns/names/{namespace}/{literal}/freeze` ебәрә.
4. I18NT000000019X раҫлай өҫтөнлөк, `NameUnfrozen` сыға.

## 7. Валидация & Хата Кодтары

| Код | Тасуирлама | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Ярлыҡ һаҡлана йәки блоклана. | 409 |
| `sns_err_policy_violation` | Термин, ярус, йәки контроллер сәйәсәтте боҙа. | 422 |
| `sns_err_payment_mismatch` | Түләү иҫбатлау ҡиммәте йәки активтар тап килмәү. | 402 |
| `sns_err_governance_missing` | Кәрәкле идара итеү артефакттары юҡ/дөрөҫ түгел. | 403 |
| `sns_err_state_conflict` | Эшләнмәгән ғәмәлдәге тормош циклы хәлендә. | 409 |

Бөтә кодтар өҫтөндә `X-Iroha-Error-Code` аша өҫкә сыға һәм структуралы I18NT0000000005X JSON/NRPC конверттары.

## 8. Ғәмәлгә ашырыу Иҫкәрмәләр

- I18NI00000000126X аукциондарын көтөп торған I18NT0000020X магазиндарында туранан-тура теркәүгә ынтылыштарҙы кире ҡаға.
- Түләү иҫбатлауҙары I18NT000000006X баш китабы квитанцияларын ҡабаттан ҡуллана; ҡаҙна хеҙмәттәре ярҙамсы API-лар тәьмин итә (I18NI000000128X).
- SDKs был ос нөктәләре менән көслө типлы ярҙамсылары менән урап үтергә тейеш, шуға күрә янсыҡтар аныҡ хата сәбәптәрен тәҡдим итә ала (I18NI0000129X, һ.б.).

## 9. Киләһе аҙымдар

- I18NT000000021X сымлы обработчиктар ысын реестр килешүе бер тапҡыр SN-3 аукциондар ер.
- SDK-специфик етәкселәр баҫтырыу (Раст/JS/Swift) был API һылтанма.
- Оҙайтыу [`sns_suffix_governance_charter.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) идара итеү ҡармаҡ дәлилдәре баҫыуҙарына һылтанмалар менән.