---
lang: hy
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2025-12-29T18:16:35.933651+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Հաճախորդի API-ի կազմաձևման տեղեկանք

Այս փաստաթուղթը հետևում է Torii հաճախորդին ուղղված կոնֆիգուրացիայի կոճակներին, որոնք
մակերեսները `iroha_config::parameters::user::Torii`-ի միջոցով: Ստորև բերված հատվածը
կենտրոնանում է NRPC-1-ի համար ներդրված Norito-RPC տրանսպորտային հսկիչների վրա. ապագան
հաճախորդի API-ի կարգավորումները պետք է ընդլայնեն այս ֆայլը:

### `torii.transport.norito_rpc`

| Բանալի | Տեսակ | Կանխադրված | Նկարագրություն |
|-----|------|---------|-------------|
| `enabled` | `bool` | `true` | Հիմնական անջատիչ, որը հնարավորություն է տալիս երկուական Norito ապակոդավորումը: Երբ `false`, Torii-ը մերժում է յուրաքանչյուր Norito-RPC հարցումը `403 norito_rpc_disabled`-ով: |
| `stage` | `string` | `"disabled"` | Տարածման մակարդակ՝ `disabled`, `canary` կամ `ga`: Փուլերը խթանում են ընդունելության որոշումները և `/rpc/capabilities` ելքը: |
| `require_mtls` | `bool` | `false` | Կիրառում է mTLS Norito-RPC փոխադրման համար. երբ `true`, Torii-ը մերժում է Norito-RPC հարցումները, որոնք չունեն mTLS նշիչի վերնագիր (օրինակ՝ I1800300): Դրոշը հայտնվում է `/rpc/capabilities`-ի միջոցով, որպեսզի SDK-ները կարողանան զգուշացնել սխալ կազմաձևված միջավայրերի մասին: |
| `allowed_clients` | `array<string>` | `[]` | Canary թույլտվության ցուցակ. Երբ `stage = "canary"`, ընդունվում են միայն այս ցանկում առկա `X-API-Token` վերնագիր կրող հարցումները: |

Օրինակ կազմաձևում.

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

Բեմական իմաստաբանություն.

- **անջատված** — Norito-RPC-ն անհասանելի է, նույնիսկ եթե `enabled = true`: Հաճախորդներ
  ստանալ `403 norito_rpc_disabled`:
- **canary** — Հարցումները պետք է ներառեն մեկին համապատասխանող `X-API-Token` վերնագիր
  `allowed_clients`-ից: Մնացած բոլոր հարցումները ստանում են `403
  norito_rpc_canary_denied`:
- **ga** — Norito-RPC հասանելի է յուրաքանչյուր վավերացված զանգահարողի համար (ենթակա է
  սովորական տոկոսադրույքը և նախնական վավերացման սահմանները):

Օպերատորները կարող են դինամիկ կերպով թարմացնել այս արժեքները `/v1/config`-ի միջոցով: Յուրաքանչյուր փոփոխություն
անմիջապես արտացոլվում է `/rpc/capabilities`-ում՝ թույլ տալով SDK-ներ և դիտելիություն
վահանակներ՝ կենդանի տրանսպորտի կեցվածքը ցույց տալու համար: