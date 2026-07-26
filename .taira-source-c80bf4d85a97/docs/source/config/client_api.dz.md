---
lang: dz
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2025-12-29T18:16:35.933651+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## མཁོ་མངགས་ཨེ་པི་ཨའི་ རིམ་སྒྲིག་གཞི་བསྟུན།

ཡིག་ཆ་འདི་གིས་ Torii མཁོ་སྤྲོད་པ་-གདོང་སྒྲིག་རིམ་སྒྲིག་གི་མཛུབ་གནོན་ཚུ་ འཚོལ་ཞིབ་འབདཝ་ཨིན།
ཁ་ཐོག་ཚུ་ `iroha_config::parameters::user::Torii` བརྒྱུད་དེ་ཨིན། འོག་གི་དབྱེ་ཚན་ནི།
Norito-RPC སྐྱེལ་འདྲེན་ཚད་འཛིན་ NRPC-1 གི་དོན་ལུ་ གཙོ་བོར་བཏོན་ཡོདཔ་ཨིན། མ་འོངས་པ
མཁོ་སྤྲོད་འབད་མི་ཨེ་པི་ཨའི་སྒྲིག་སྟངས་ཚུ་གིས་ ཡིག་སྣོད་འདི་རྒྱ་བསྐྱེད་འབད་དགོ།

### `torii.transport.norito_rpc`

| ལྡེ་མིག་ | དབྱེ་བ་ | སྔོན་སྒྲིག་ | འགྲེལ་བཤད་ |
|--|-|-|---------------------------------------- |
| `enabled` | `bool` | `true` | གཉིས་ལྡན་ Norito ཌི་ཀོ་ཌིང་ལྕོགས་ཅན་བཟོ་མི་ མཐོ་རིམ་བསྒྱུར་བཅོས། `false` གིས་ `403 norito_rpc_disabled` དང་གཅིག་ཁར་ Norito-RPC གི་ཞུ་བ་རེ་རེ་ ངོས་ལེན་འབདཝ་ཨིན། |
| `stage` | `string` | `"disabled"` | བཤུད་བརྙན་རིམ་པ་: `disabled`, `canary`, ཡང་ན་ `ga`. གནས་རིམ་ཚུ་གིས་ འཛུལ་ཞུགས་གྲོས་ཐག་བཅད་ནི་དང་ `/rpc/capabilities` ཐོན་འབྲས་ཚུ་ཨིན། |
| `require_mtls` | `bool` | `false` | `true`, Torii གིས་ mTLS རྟགས་བཀལ་མི་ཆོག་པའི་ ཞུ་བ་ཚུ་ ངོས་ལེན་འབདཝ་ཨིན། (དཔེར་ན་ `X-Forwarded-Client-Cert`) རྒྱལ་དར་འདི་ `/rpc/capabilities` བརྒྱུད་དེ་ ཕྱིར་འཐེན་འབདཝ་ལས་ SDKs གིས་ རིམ་སྒྲིག་མ་འབད་བའི་ མཐའ་འཁོར་ཚུ་ནང་ ཉེན་བརྡ་འབད་ཚུགས། |
| `allowed_clients` | `array<string>` | `[]` | Canary ཆོག་མཆན་ཐོ་ཡིག། `stage = "canary"` འབད་བའི་སྐབས་ ཐོ་ཡིག་འདི་ནང་ `X-API-Token` མགོ་ཡིག་ཅིག་འབག་མི་ཞུ་བ་ཚུ་རྐྱངམ་ཅིག་ངོས་ལེན་འབདཝ་ཨིན། |

དཔེར་ན་ རིམ་སྒྲིག་:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

གོ་རིམ་ཡིག་བརྡ།

- **disabled** — I 18NI00000037X ཡིན་ནའང་ཐོབ་ཐུབ་མེད། མཁོ་མངགས་འབད་མི་ཚུ།
  `403 norito_rpc_disabled` ཐོབ་ཡོད།
- **canory** — ཞུ་བ་ཚུ་ནང་ གཅིག་དང་མཐུན་པའི་ `X-API-Token` མགོ་ཡིག་ཅིག་ བཙུགས་དགོ།
  `allowed_clients` གི་. གཞན་ཞུ་བ་ཚང་མ་ `403ཐོབ་ཡོད།
  norito_rpc_canary_danied`.
- **ga** — Norito-RPC འདི་ བདེན་བཤད་འབད་ཡོད་པའི་ འབོད་བརྡ་ག་ར་ལུ་ འཐོབ་ཚུགས།
  སྤྱིར་བཏང་ཚད་གཞི་དང་ བདེན་བཤད་ཚད་གཞི་)།

བཀོལ་སྤྱོད་པ་ཚུ་གིས་ གནས་གོང་འདི་ཚུ་ `/v1/config` བརྒྱུད་དེ་ མགྱོགས་དྲགས་སྦེ་དུས་མཐུན་བཟོ་ཚུགས། འགྱུར་རེ།
འདི་འཕྲལ་ལས་ `/rpc/capabilities` ནང་ལུ་ བསྟན་ཏེ་ SDKs དང་ བལྟ་རྟོག་འབད་ཚུགསཔ་ཨིན།
dashboods གིས་ ཐད་གཏོང་སྐྱེལ་འདྲེན་གྱི་གནས་སྟངས་སྟོན་ནི།