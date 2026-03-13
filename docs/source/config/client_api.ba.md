---
lang: ba
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2025-12-29T18:16:35.933651+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Клиент API конфигурацияһы һылтанмаһы

Был документ Torii клиент-йөҙөндә конфигурация ручкаларын күҙәтә, улар
ер өҫтө `iroha_config::parameters::user::Torii` аша. Түбәндәге бүлек
Norito транспорт менән идара итеүгә йүнәлтелгән NRPC-1 өсөн индерелгән; киләсәк
клиент API параметрҙары был файлды киңәйтергә тейеш.

### `torii.transport.norito_rpc`

| Асҡыс | Тип | Ғәҙәттәгесә | Тасуирлама |
|----|-----|----------|------------- |
| `enabled` | `bool` | `true` | Мастер-карта, мөмкинлек бирә бинар Norito декодлау. Ҡасан `false`, Torii һәр Norito-RPC запросы менән `403 norito_rpc_disabled` кире ҡаға. |
| `stage` | `string` | `"disabled"` | Rollout ярус: `disabled`, `canary`, йәки `ga`. Этаптар ҡабул итеү ҡарарҙары һәм `/rpc/capabilities` продукцияһы драйв. |
| `require_mtls` | `bool` | `false` | Norito-RPC транспорты өсөн mTLS үтәй: `true`, Torii Norito-RPC запростарын кире ҡаға, улар мТЛС маркеры башын йөрөтмәй (мәҫәлән, `X-Forwarded-Client-Cert`). Флаг `/rpc/capabilities` аша сыға, шуға күрә SDKs дөрөҫ булмаған конфигурацияланмаған мөхиттәр тураһында иҫкәртә ала. |
| `allowed_clients` | `array<string>` | `[]` | Канар рөхсәт ҡағыҙы. Ҡасан `stage = "canary"`, был исемлектә `X-API-Token` башын йөрөтөү тураһында ғына үтенестәр ҡабул ителә. |

Миҫал конфигурацияһы:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

Этап семантикаһы:

- **инвалид** — Norito-RPC, хатта `enabled = true`. Клиенттар
  `403 norito_rpc_disabled` ала.
- **канар** — Запростар `X-API-Token` X башын үҙ эсенә алырға тейеш, ул бер тап килә
  `allowed_clients`. Ҡалған бөтә үтенестәр ҙә `403 ала.
  норито_рпц_канар_күҙәлгән».
- **га** — Norito-RPC һәр аутентификацияланған шылтыратыусы өсөн мөмкин (унда
  ғәҙәти ставкаһы һәм алдан аут сиктәре).

Операторҙар был ҡиммәттәрҙе динамик яңырта ала, `/v2/config` аша. Һәр үҙгәреш
тиҙ арала `/rpc/capabilities`-та сағыла, был SDK-лар һәм күҙәтеүсәнлеккә мөмкинлек бирә
приборҙар таҡталары тере транспорт поза күрһәтеү өсөн.