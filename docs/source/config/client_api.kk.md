---
lang: kk
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2025-12-29T18:16:35.933651+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Клиент API конфигурациясының анықтамасы

Бұл құжат Torii клиентке бағытталған конфигурация тұтқаларын бақылайды.
`iroha_config::parameters::user::Torii` арқылы беттер. Төмендегі бөлім
NRPC-1 үшін енгізілген Norito-RPC көлік басқару элементтеріне назар аударады; болашақ
клиент API параметрлері бұл файлды кеңейтуі керек.

### `torii.transport.norito_rpc`

| Негізгі | |түрі Әдепкі | Сипаттама |
|-----|------|---------|-------------|
| `enabled` | `bool` | `true` | Norito екілік декодтауды қосатын негізгі қосқыш. `false` кезде, Torii `403 norito_rpc_disabled` арқылы әрбір Norito-RPC сұрауын қабылдамайды. |
| `stage` | `string` | `"disabled"` | Шығарылым деңгейі: `disabled`, `canary` немесе `ga`. Кезеңдер қабылдау шешімдерін және `/rpc/capabilities` шығысын басқарады. |
| `require_mtls` | `bool` | `false` | Norito-RPC тасымалдау үшін mTLS күшіне енеді: `true`, Torii mTLS маркер тақырыбын (мысалы, I18000X) тасымалдамайтын Norito-RPC сұрауларын қабылдамайды. Жалау `/rpc/capabilities` арқылы көрсетіледі, сондықтан SDK қате конфигурацияланған орталар туралы ескертеді. |
| `allowed_clients` | `array<string>` | `[]` | Канарияның рұқсат етілген тізімі. `stage = "canary"` болғанда, осы тізімде бар `X-API-Token` тақырыбы бар сұраулар ғана қабылданады. |

Мысал конфигурация:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

Кезең семантикасы:

- **өшірілген** — Norito-RPC тіпті `enabled = true` болса да қолжетімді емес. Клиенттер
  `403 norito_rpc_disabled` алыңыз.
- **канар** — Сұраулар біріне сәйкес келетін `X-API-Token` тақырыбын қамтуы керек.
  `allowed_clients`. Барлық басқа сұраулар `403 алады
  norito_rpc_canary_denied`.
- **ga** — Norito-RPC әрбір аутентификацияланған қоңырау шалушыға қолжетімді (шарт бойынша
  әдеттегі мөлшерлеме және алдын ала авторизациялық шектеулер).

Операторлар бұл мәндерді `/v2/config` арқылы динамикалық түрде жаңарта алады. Әрбір өзгеріс
бірден `/rpc/capabilities` ішінде көрсетіледі, SDK және бақылау мүмкіндігін береді
тікелей көлік қалпын көрсететін бақылау тақталары.