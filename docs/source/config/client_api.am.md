---
lang: am
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2025-12-29T18:16:35.933651+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## የደንበኛ ኤፒአይ ውቅር ማጣቀሻ

ይህ ሰነድ Torii ደንበኛን የሚመለከቱ የማዋቀሪያ ቁልፎችን ይከታተላል
ወለል በ `iroha_config::parameters::user::Torii`. ከታች ያለው ክፍል
ለ NRPC-1 በተዋወቁት Norito-RPC የትራንስፖርት መቆጣጠሪያዎች ላይ ያተኩራል; ወደፊት
የደንበኛ ኤፒአይ ቅንጅቶች ይህን ፋይል ማራዘም አለባቸው።

### `torii.transport.norito_rpc`

| ቁልፍ | አይነት | ነባሪ | መግለጫ |
|--------|-----|------------|
| `enabled` | `bool` | `true` | ሁለትዮሽ Norito መፍታትን የሚያስችል ማስተር ማብሪያ / ማጥፊያ። `false`፣ Torii እያንዳንዱን የNorito-RPC ጥያቄ በ`403 norito_rpc_disabled` ውድቅ ያደርጋል። |
| `stage` | `string` | `"disabled"` | የልቀት ደረጃ፡ `disabled`፣ `canary`፣ ወይም `ga`። ደረጃዎች የመግቢያ ውሳኔዎችን እና የ `/rpc/capabilities` ውፅዓትን ያመጣሉ ። |
| `require_mtls` | `bool` | `false` | mTLSን ለNorito-RPC ትራንስፖርት ያስፈጽማል፡ `true`፣ Torii የmTLS ማርከር ርዕስ (ለምሳሌ I18000000) የማይይዙ የNorito-RPC ጥያቄዎችን ውድቅ ያደርጋል። ባንዲራ በ`/rpc/capabilities` በኩል ወጥቷል ስለዚህ ኤስዲኬዎች በተሳሳተ መንገድ የተዋቀሩ አካባቢዎችን ያስጠነቅቃሉ። |
| `allowed_clients` | `array<string>` | `[]` | የካናሪ የተፈቀደ ዝርዝር። `stage = "canary"` ሲሆን በዚህ ዝርዝር ውስጥ የ`X-API-Token` ራስጌ ይዘው የሚቀርቡ ጥያቄዎች ብቻ ይቀበላሉ። |

የምሳሌ ውቅር፡

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

የደረጃ ትርጉም፡

- ** ተሰናክሏል *** - Norito-RPC ምንም እንኳን `enabled = true` አይገኝም። ደንበኞች
  `403 norito_rpc_disabled` ተቀበል።
- ** ካናሪ *** - ጥያቄዎች ከአንድ ጋር የሚዛመድ `X-API-Token` ራስጌ ማካተት አለባቸው
  የ `allowed_clients`. ሁሉም ሌሎች ጥያቄዎች `403 ይቀበላሉ።
  norito_rpc_ካናሪ_ካድ'
- ** ga** — Norito-RPC ለእያንዳንዱ የተረጋገጠ ደዋይ ይገኛል (ለ
  መደበኛ ተመን እና ቅድመ-የተረጋገጠ ገደቦች)።

ኦፕሬተሮች እነዚህን እሴቶች በተለዋዋጭ በ`/v2/config` በኩል ማዘመን ይችላሉ። እያንዳንዱ ለውጥ
ኤስዲኬዎችን እና ታዛቢነትን በመፍቀድ በ `/rpc/capabilities` ውስጥ ወዲያውኑ ይንጸባረቃል
የቀጥታ መጓጓዣ አቀማመጥን ለማሳየት ዳሽቦርዶች።