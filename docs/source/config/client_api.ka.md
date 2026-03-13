---
lang: ka
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2025-12-29T18:16:35.933651+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## კლიენტის API კონფიგურაციის მითითება

ეს დოკუმენტი თვალყურს ადევნებს Torii კლიენტის მიმართულ კონფიგურაციის ღილაკებს, რომლებიც
ზედაპირები `iroha_config::parameters::user::Torii`-ით. განყოფილება ქვემოთ
ფოკუსირებულია Norito-RPC სატრანსპორტო სამართავებზე, რომლებიც დანერგილია NRPC-1-ისთვის; მომავალი
კლიენტის API პარამეტრები უნდა გააფართოვოს ეს ფაილი.

### `torii.transport.norito_rpc`

| გასაღები | ტიპი | ნაგულისხმევი | აღწერა |
|-----|------|---------|-------------|
| `enabled` | `bool` | `true` | ძირითადი გადამრთველი, რომელიც იძლევა ორობითი Norito დეკოდირების საშუალებას. როდესაც `false`, Torii უარყოფს ყოველ Norito-RPC მოთხოვნას `403 norito_rpc_disabled`-ით. |
| `stage` | `string` | `"disabled"` | გაშვების დონე: `disabled`, `canary`, ან `ga`. ეტაპები განაპირობებს დაშვების გადაწყვეტილებებს და `/rpc/capabilities` გამომავალს. |
| `require_mtls` | `bool` | `false` | ახორციელებს mTLS Norito-RPC ტრანსპორტისთვის: როდესაც `true`, Torii უარყოფს Norito-RPC მოთხოვნებს, რომლებიც არ შეიცავს mTLS მარკერის სათაურს (მაგ. I1800300). დროშა გამოჩნდება `/rpc/capabilities`-ის მეშვეობით, რათა SDK-ებმა გააფრთხილონ არასწორ კონფიგურაციულ გარემოში. |
| `allowed_clients` | `array<string>` | `[]` | კანარის ნებადართული სია. როდესაც `stage = "canary"`, მიიღება მხოლოდ მოთხოვნები ამ სიაში არსებული `X-API-Token` სათაურის შემცველობით. |

კონფიგურაციის მაგალითი:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

ეტაპის სემანტიკა:

- **გამორთულია** — Norito-RPC მიუწვდომელია მაშინაც კი, თუ `enabled = true`. კლიენტები
  მიიღეთ `403 norito_rpc_disabled`.
- **კანარი** — მოთხოვნები უნდა შეიცავდეს `X-API-Token` სათაურს, რომელიც ემთხვევა ერთს
  `allowed_clients`-ის. ყველა სხვა მოთხოვნა მიიღება `403
  norito_rpc_canary_nnied`.
- **ga** — Norito-RPC ხელმისაწვდომია ყველა ავტორიზებული აბონენტისთვის (ექვემდებარება
  ჩვეულებრივი მაჩვენებელი და წინასწარი ავტორიზაციის ლიმიტები).

ოპერატორებს შეუძლიათ ამ მნიშვნელობების დინამიურად განახლება `/v2/config`-ის საშუალებით. ყოველი ცვლილება
დაუყოვნებლივ აისახება `/rpc/capabilities`-ში, რაც საშუალებას აძლევს SDK-ებს და დაკვირვებას
დაფები ცოცხალი ტრანსპორტის პოზის საჩვენებლად.