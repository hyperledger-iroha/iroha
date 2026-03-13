---
lang: mn
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2025-12-29T18:16:35.933651+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Client API тохиргооны лавлагаа

Энэ баримт бичиг нь Torii үйлчлүүлэгч рүү чиглэсэн тохиргооны товчлууруудыг дагаж мөрддөг.
гадаргуу `iroha_config::parameters::user::Torii` дамжуулан. Доорх хэсэг
NRPC-1-д нэвтрүүлсэн Norito-RPC тээврийн удирдлагад анхаарлаа хандуулдаг; ирээдүй
клиент API тохиргоо нь энэ файлыг өргөтгөх ёстой.

### `torii.transport.norito_rpc`

| Түлхүүр | Төрөл | Өгөгдмөл | Тодорхойлолт |
|-----|------|---------|-------------|
| `enabled` | `bool` | `true` | Хоёртын Norito код тайлахыг идэвхжүүлдэг мастер шилжүүлэгч. `false` үед Torii нь `403 norito_rpc_disabled`-тай Norito-RPC хүсэлт бүрээс татгалздаг. |
| `stage` | `string` | `"disabled"` | Дамжуулах шат: `disabled`, `canary`, эсвэл `ga`. Үе шатууд элсэлтийн шийдвэр болон `/rpc/capabilities` гаралтыг удирддаг. |
| `require_mtls` | `bool` | `false` | Norito-RPC зөөвөрлөхөд mTLS-ийг хэрэгжүүлдэг: `true` үед Torii нь mTLS тэмдэглэгээний толгой агуулаагүй Norito-RPC хүсэлтээс татгалздаг (жишээ нь, I000030X). Дарцаг нь `/rpc/capabilities`-ээр гарч ирдэг тул SDK нь буруу тохируулагдсан орчинд анхааруулах боломжтой. |
| `allowed_clients` | `array<string>` | `[]` | Канарын зөвшөөрөгдсөн жагсаалт. `stage = "canary"` үед зөвхөн энэ жагсаалтад байгаа `X-API-Token` гарчигтай хүсэлтийг хүлээн авна. |

Жишээ тохиргоо:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

Тайзны семантик:

- **идэвхгүй** — Norito-RPC нь `enabled = true` байсан ч боломжгүй. Үйлчлүүлэгчид
  `403 norito_rpc_disabled` хүлээн авах.
- **канар** — Хүсэлтэд нэгтэй таарах `X-API-Token` толгой хэсгийг агуулсан байх ёстой.
  `allowed_clients`. Бусад бүх хүсэлтүүд `403-ыг хүлээн авдаг
  norito_rpc_canary_denied`.
- **ga** — Norito-RPC нь баталгаажсан дуудлага хийгч бүрт боломжтой (хэрэгтэй.
  ердийн хувь хэмжээ ба баталгаажуулалтын өмнөх хязгаарлалт).

Операторууд `/v2/config`-ээр дамжуулан эдгээр утгыг динамикаар шинэчлэх боломжтой. Өөрчлөлт бүр
`/rpc/capabilities`-д нэн даруй тусгагдсан бөгөөд SDK болон ажиглагдах боломжтой
шууд тээврийн байрлалыг харуулах хяналтын самбар.